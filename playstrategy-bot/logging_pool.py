import logging
from multiprocessing.pool import Pool

logger = logging.getLogger(__name__)


# Shortcut to multiprocessing's logger
def error(msg, *args):
    return logger.error(msg, *args)


class LogExceptions:
    def __init__(self, callable):
        self.__callable = callable

    def __call__(self, *args, **kwargs):
        logger.debug(
            f"LogExceptions: Calling {self.__callable} with args={args}, kwargs={kwargs}"
        )
        try:
            result = self.__callable(*args, **kwargs)
            logger.debug(f"LogExceptions: {self.__callable} completed successfully")
            return result
        except Exception:
            logger.exception(
                f"LogExceptions: Exception in worker process executing {self.__callable}"
            )
            raise


class LoggingPool(Pool):
    def apply_async(self, func, args=(), kwds={}, callback=None):
        logger.debug(
            f"LoggingPool: apply_async called for {func} with args={args}, kwds={kwds}"
        )
        return Pool.apply_async(self, LogExceptions(func), args, kwds, callback)
